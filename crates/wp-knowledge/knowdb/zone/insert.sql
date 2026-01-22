INSERT INTO {table} (start_ip_int, end_ip_int, zone)
VALUES (
    ip4_int(trim_quotes(?1)),
    ip4_int(trim_quotes(?2)),
    trim_quotes(?3)
);
