//! Coordinates the test caching workflow.

use crate::{
    execution::{self, CapturedOutput},
    identity::{CacheKey, FileIdentity},
    invocation::Invocation,
    reporting::{self, Outcome},
    storage::{Cache, SqliteCache},
};
use anyhow::{Context, Result};

pub fn run(invocation: Invocation) -> Result<i32> {
    let binary = match FileIdentity::read(invocation.working_dir.join(&invocation.executable)) {
        Ok(binary) => binary,
        Err(error) => return on_cache_unavailable(&invocation, error),
    };
    let key = CacheKey::new(
        &binary,
        &invocation.binary_id,
        &invocation.test_name,
        &invocation.working_dir,
        &invocation.workspace,
        &invocation.args,
    );
    let cache = match SqliteCache::open(&invocation.cache_dir) {
        Ok(cache) => cache,
        Err(error) => return on_cache_unavailable(&invocation, error),
    };

    if invocation.force {
        return on_force(&invocation, &cache, &key);
    }

    let cached_result = cache
        .get(&key)
        .with_context(|| format!("loading cached result for {}", binary.path.display()));
    match cached_result {
        Ok(Some(output)) => on_cache_hit(&invocation, &output),
        Ok(None) => on_cache_miss(&invocation, &cache, &key),
        Err(error) => {
            drop(cache);
            on_cache_unavailable(&invocation, error)
        }
    }
}

fn on_force(invocation: &Invocation, cache: &impl Cache, key: &CacheKey) -> Result<i32> {
    let result = execute_and_update_cache(invocation, cache, key)?;
    match result.cache_update {
        Ok(()) => {
            reporting::write(invocation.report_path.as_deref(), Outcome::Forced);
            Ok(result.exit_code)
        }
        Err(error) => on_cache_update_failed(invocation, error, result.exit_code),
    }
}

fn on_cache_hit(invocation: &Invocation, output: &CapturedOutput) -> Result<i32> {
    execution::replay(output)?;
    reporting::write(invocation.report_path.as_deref(), Outcome::Cached);
    Ok(0)
}

fn on_cache_miss(invocation: &Invocation, cache: &impl Cache, key: &CacheKey) -> Result<i32> {
    let result = execute_and_update_cache(invocation, cache, key)?;
    match result.cache_update {
        Ok(()) => {
            reporting::write(invocation.report_path.as_deref(), Outcome::CacheMiss);
            Ok(result.exit_code)
        }
        Err(error) => on_cache_update_failed(invocation, error, result.exit_code),
    }
}

fn on_cache_unavailable(invocation: &Invocation, error: anyhow::Error) -> Result<i32> {
    eprintln!("prevtest: cache unavailable, executing test: {error:#}");
    let executed = execution::run(&invocation.executable, &invocation.args)?;
    reporting::write(invocation.report_path.as_deref(), Outcome::CacheUnavailable);
    Ok(executed.exit_code())
}

fn on_cache_update_failed(
    invocation: &Invocation,
    error: anyhow::Error,
    exit_code: i32,
) -> Result<i32> {
    eprintln!("prevtest: could not update cache: {error:#}");
    reporting::write(
        invocation.report_path.as_deref(),
        Outcome::CacheUpdateFailed,
    );
    Ok(exit_code)
}

struct RunResult {
    exit_code: i32,
    cache_update: Result<()>,
}

fn execute_and_update_cache(
    invocation: &Invocation,
    cache: &impl Cache,
    key: &CacheKey,
) -> Result<RunResult> {
    let executed = execution::run(&invocation.executable, &invocation.args)?;
    let cache_update = if executed.status.success() {
        cache.put(
            key,
            &invocation.binary_id,
            &invocation.test_name,
            &executed.output,
        )
    } else {
        cache.remove(key)
    };
    Ok(RunResult {
        exit_code: executed.exit_code(),
        cache_update,
    })
}
