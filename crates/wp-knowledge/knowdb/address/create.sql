CREATE TABLE IF NOT EXISTS {table} (
  id    INTEGER PRIMARY KEY,
  value TEXT NOT NULL
);

-- 可选索引：提升按 value 的查找
CREATE INDEX IF NOT EXISTS idx_{table}_value ON {table}(value);

