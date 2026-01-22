CREATE TABLE IF NOT EXISTS {table} (
  id            INTEGER PRIMARY KEY,
  start_ip_int  INTEGER NOT NULL,
  end_ip_int    INTEGER NOT NULL,
  zone          TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_{table}_start ON {table}(start_ip_int);
CREATE INDEX IF NOT EXISTS idx_{table}_end   ON {table}(end_ip_int);
