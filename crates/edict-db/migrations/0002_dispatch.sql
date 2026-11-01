-- Ausführungssperren für das Gateway.
--
-- Jeder Ausführungsauftrag wird mit einer Sperre versehen. Sie hält fest,
-- welcher Worker den Auftrag ausführt, wie lange die Sperre gilt und wie
-- viele Versuche es gab. Ein Auftrag läuft damit höchstens einmal gleichzeitig.

CREATE TABLE dispatch_leases (
    task_id UUID NOT NULL REFERENCES tasks(task_id) ON DELETE CASCADE,
    request_id UUID NOT NULL,
    attempt INTEGER NOT NULL DEFAULT 1,
    dispatch_id TEXT NOT NULL DEFAULT '',
    owner TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'running'
        CHECK (status IN ('running', 'succeeded', 'failed')),
    lease_until TIMESTAMPTZ NOT NULL,
    last_error TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (task_id, request_id)
);

CREATE INDEX dispatch_leases_lease_idx ON dispatch_leases (lease_until);
