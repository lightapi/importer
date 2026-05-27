use crate::cli::ImportArgs;
use crate::db::{Database, InsertResult};
use crate::event::cloud_event::{normalize_event, NormalizedEvent};
use crate::event::mutator::EventMutator;
use crate::io::{read_rule_arg, read_to_string};
use anyhow::{anyhow, Result};
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub file: String,
    pub total: usize,
    pub imported: usize,
    pub validated: usize,
    pub skipped_duplicate_input: usize,
    pub skipped_existing_target: usize,
    pub skipped_exact_duplicate: usize,
    pub failed: usize,
}

pub async fn run_import(args: ImportArgs, db: Option<&Database>) -> Result<ImportSummary> {
    if args.batch_size == 0 {
        return Err(anyhow!("--batch-size must be greater than zero"));
    }
    if !args.dry_run && db.is_none() {
        return Err(anyhow!(
            "database connection is required unless --dry-run is set"
        ));
    }

    let replacement_json = optional_rule_json(args.replacement.as_deref()).await?;
    let enrichment_json = optional_rule_json(args.enrichment.as_deref()).await?;
    let mut mutator = EventMutator::new(replacement_json.as_deref(), enrichment_json.as_deref())?;
    let recompute_subject_after_mutation = mutator.has_replacements();

    let content = read_to_string(&args.filename).await?;
    let raw_events: Vec<Value> = serde_json::from_str(&content)
        .map_err(|err| anyhow!("input file is not a valid JSON event array: {err}"))?;

    let mut summary = ImportSummary {
        file: args.filename.clone(),
        total: raw_events.len(),
        ..ImportSummary::default()
    };

    let mut seen_versions = HashSet::new();
    let mut aggregate_version_cache = HashMap::<String, i64>::new();
    let mut batch = Vec::<NormalizedEvent>::with_capacity(args.batch_size);

    for (idx, mut raw_event) in raw_events.into_iter().enumerate() {
        match prepare_event(
            &mut raw_event,
            &mut mutator,
            recompute_subject_after_mutation,
            &mut seen_versions,
            &mut aggregate_version_cache,
            db,
        )
        .await
        {
            Ok(PrepareOutcome::Ready(event)) => {
                summary.validated += 1;
                if args.dry_run {
                    continue;
                }
                batch.push(event);
                if batch.len() >= args.batch_size {
                    flush_batch(&mut batch, db.expect("checked above"), &args, &mut summary)
                        .await?;
                }
            }
            Ok(PrepareOutcome::SkippedDuplicateInput) => {
                summary.skipped_duplicate_input += 1;
            }
            Ok(PrepareOutcome::SkippedExistingTarget) => {
                summary.skipped_existing_target += 1;
            }
            Err(err) => {
                summary.failed += 1;
                error!(event_index = idx, error = %err, "failed to prepare event");
                if args.fail_fast {
                    return Err(err);
                }
            }
        }
    }

    if !batch.is_empty() {
        flush_batch(&mut batch, db.expect("checked above"), &args, &mut summary).await?;
    }

    info!(
        imported = summary.imported,
        skipped_duplicate_input = summary.skipped_duplicate_input,
        skipped_existing_target = summary.skipped_existing_target,
        skipped_exact_duplicate = summary.skipped_exact_duplicate,
        failed = summary.failed,
        "import finished"
    );

    Ok(summary)
}

async fn optional_rule_json(value: Option<&str>) -> Result<Option<String>> {
    match value {
        Some(value) => Ok(Some(read_rule_arg(value).await?)),
        None => Ok(None),
    }
}

enum PrepareOutcome {
    Ready(NormalizedEvent),
    SkippedDuplicateInput,
    SkippedExistingTarget,
}

async fn prepare_event(
    raw_event: &mut Value,
    mutator: &mut EventMutator,
    recompute_subject_after_mutation: bool,
    seen_versions: &mut HashSet<String>,
    aggregate_version_cache: &mut HashMap<String, i64>,
    db: Option<&Database>,
) -> Result<PrepareOutcome> {
    mutator.mutate(raw_event);
    let event = normalize_event(raw_event.clone(), recompute_subject_after_mutation)?;
    let version_key = format!("{}|{}", event.aggregate_id, event.aggregate_version);
    if !seen_versions.insert(version_key) {
        debug!(
            aggregate_id = event.aggregate_id,
            aggregate_version = event.aggregate_version,
            "skipping duplicate aggregate version in input"
        );
        return Ok(PrepareOutcome::SkippedDuplicateInput);
    }

    if let Some(db) = db {
        let existing_version =
            if let Some(version) = aggregate_version_cache.get(&event.aggregate_id) {
                *version
            } else {
                let version = db.max_aggregate_version(&event.aggregate_id).await?;
                aggregate_version_cache.insert(event.aggregate_id.clone(), version);
                version
            };

        if existing_version >= event.aggregate_version {
            debug!(
                aggregate_id = event.aggregate_id,
                aggregate_version = event.aggregate_version,
                existing_version,
                "skipping existing target aggregate version"
            );
            return Ok(PrepareOutcome::SkippedExistingTarget);
        }
        aggregate_version_cache.insert(event.aggregate_id.clone(), event.aggregate_version);
    }

    Ok(PrepareOutcome::Ready(event))
}

async fn flush_batch(
    batch: &mut Vec<NormalizedEvent>,
    db: &Database,
    args: &ImportArgs,
    summary: &mut ImportSummary,
) -> Result<()> {
    if batch.is_empty() {
        return Ok(());
    }

    if batch.len() == 1 {
        insert_one(&mut batch[0], db, args, summary).await?;
        batch.clear();
        return Ok(());
    }

    match db.insert_events(batch).await {
        Ok(InsertResult::Inserted(count)) => {
            summary.imported += count;
        }
        Ok(InsertResult::SkippedExactDuplicate) => {
            summary.skipped_exact_duplicate += 1;
        }
        Err(err) if !args.fail_fast => {
            warn!(error = %err, "batch insert failed; retrying events one at a time");
            for event in batch.iter_mut() {
                insert_one(event, db, args, summary).await?;
            }
        }
        Err(err) => return Err(err),
    }

    batch.clear();
    Ok(())
}

async fn insert_one(
    event: &mut NormalizedEvent,
    db: &Database,
    args: &ImportArgs,
    summary: &mut ImportSummary,
) -> Result<()> {
    match db.insert_events(std::slice::from_mut(event)).await {
        Ok(InsertResult::Inserted(_)) => {
            summary.imported += 1;
        }
        Ok(InsertResult::SkippedExactDuplicate) => {
            summary.skipped_exact_duplicate += 1;
        }
        Err(err) => {
            summary.failed += 1;
            error!(
                event_id = %event.id,
                aggregate_id = event.aggregate_id,
                aggregate_version = event.aggregate_version,
                error = %err,
                "failed to insert event"
            );
            if args.fail_fast {
                return Err(err);
            }
        }
    }
    Ok(())
}
