-- REPLICA IDENTITY DEFAULT (mặc định của Postgres, phổ biến nhất ở production):
-- message delete chỉ có khóa chính trong "before", các cột NOT NULL khác bị null.
CREATE TABLE members (
    id     integer PRIMARY KEY,
    name   text NOT NULL,
    status text NOT NULL
);

INSERT INTO members VALUES (1, 'a', 'active'), (2, 'b', 'active'), (3, 'c', 'active');
