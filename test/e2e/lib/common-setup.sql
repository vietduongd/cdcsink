-- Table đánh dấu: runner insert một token rồi chờ token xuất hiện ở đích,
-- nghĩa là mọi thay đổi trước đó đã được cdcsink xử lý.
CREATE TABLE e2e_marker (
    id         text PRIMARY KEY,
    created_at timestamptz NOT NULL DEFAULT now()
);
ALTER TABLE e2e_marker REPLICA IDENTITY FULL;
