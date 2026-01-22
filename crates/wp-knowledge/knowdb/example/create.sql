CREATE TABLE IF NOT EXISTS {table} (
  id      INTEGER PRIMARY KEY,
  name    TEXT NOT NULL,
  pinying TEXT NOT NULL
);

-- 可选索引：按 name 查询更快
CREATE INDEX IF NOT EXISTS idx_{table}_name ON {table}(name);
