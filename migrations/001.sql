CREATE TABLE passed_test_results (
    cache_key BLOB NOT NULL PRIMARY KEY CHECK(length(cache_key) = 32),
    binary_id TEXT NOT NULL,
    name TEXT NOT NULL,
    stdout BLOB NOT NULL,
    stderr BLOB NOT NULL
);
CREATE INDEX test_results_by_test ON passed_test_results(binary_id, name);
