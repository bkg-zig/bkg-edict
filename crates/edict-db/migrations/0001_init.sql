-- Kanonisches Schema des Edict-Port.
--
-- Die Kennungen bleiben getrennt: `task_id` ist die technische UUID, die
-- Ereignisse und Worker verbindet. `legacy_id` ist die fachliche Kennung im
-- Format `JJC-YYYYMMDD-NNN` und bleibt für Menschen und Altbestand lesbar.
-- `trace_id` verbindet die Ereigniskette eines Vorgangs.

CREATE TABLE tasks (
    task_id UUID PRIMARY KEY,
    legacy_id TEXT NOT NULL UNIQUE,
    trace_id TEXT NOT NULL DEFAULT '',
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    state TEXT NOT NULL DEFAULT 'Taizi'
        CHECK (state IN (
            'Taizi', 'Zhongshu', 'Menxia', 'Assigned', 'Next',
            'Doing', 'Review', 'Done', 'Blocked', 'Cancelled', 'Pending'
        )),
    org TEXT NOT NULL DEFAULT '',
    official TEXT NOT NULL DEFAULT '',
    assignee_org TEXT,
    creator TEXT NOT NULL DEFAULT 'emperor',
    now_text TEXT NOT NULL DEFAULT '',
    eta TEXT NOT NULL DEFAULT '',
    block TEXT NOT NULL DEFAULT '',
    output TEXT NOT NULL DEFAULT '',
    ac TEXT NOT NULL DEFAULT '',
    priority TEXT NOT NULL DEFAULT 'normal',
    archived BOOLEAN NOT NULL DEFAULT FALSE,
    flow_log JSONB NOT NULL DEFAULT '[]'::jsonb,
    tags JSONB NOT NULL DEFAULT '[]'::jsonb,
    meta JSONB NOT NULL DEFAULT '{}'::jsonb,
    template_id TEXT NOT NULL DEFAULT '',
    template_params JSONB NOT NULL DEFAULT '{}'::jsonb,
    target_dept TEXT NOT NULL DEFAULT '',
    scheduler JSONB NOT NULL DEFAULT '{}'::jsonb,
    review_round BIGINT NOT NULL DEFAULT 0,
    source_meta JSONB,
    prev_state TEXT,
    activity JSONB NOT NULL DEFAULT '[]'::jsonb,
    version BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    archived_at TIMESTAMPTZ
);

CREATE INDEX ix_tasks_state ON tasks (state);
CREATE INDEX ix_tasks_org ON tasks (org);
CREATE INDEX ix_tasks_assignee_org ON tasks (assignee_org);
CREATE INDEX ix_tasks_trace_id ON tasks (trace_id);
CREATE INDEX ix_tasks_archived ON tasks (archived);
CREATE INDEX ix_tasks_updated_at ON tasks (updated_at DESC);
CREATE INDEX ix_tasks_created_at ON tasks (created_at DESC);
CREATE INDEX ix_tasks_priority ON tasks (priority);
CREATE INDEX ix_tasks_archived_state_updated
    ON tasks (archived, state, updated_at DESC);
CREATE INDEX ix_tasks_tags ON tasks USING GIN (tags);
CREATE INDEX ix_tasks_meta ON tasks USING GIN (meta);

-- Teilaufgaben. Die Reihenfolge entsteht aus `position`. `legacy_id` hält die
-- Kennung aus dem Altbestand, weil diese dort Zahlen oder Zeichenketten sind.
CREATE TABLE todos (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    task_id UUID NOT NULL REFERENCES tasks (task_id) ON DELETE CASCADE,
    position INTEGER NOT NULL DEFAULT 0,
    legacy_id TEXT NOT NULL DEFAULT '',
    title TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'not-started'
        CHECK (status IN ('not-started', 'in-progress', 'completed')),
    detail TEXT
);

CREATE UNIQUE INDEX ux_todos_task_position ON todos (task_id, position);
CREATE INDEX ix_todos_task_id ON todos (task_id, position);
CREATE INDEX ix_todos_status ON todos (status);

-- Gedanken und Fortschrittsmeldungen. `payload` hält den vollständigen
-- Fortschrittseintrag, die Spalten daneben dienen der Auswertung.
CREATE TABLE thoughts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    task_id UUID NOT NULL REFERENCES tasks (task_id) ON DELETE CASCADE,
    seq BIGSERIAL NOT NULL,
    agent TEXT NOT NULL DEFAULT '',
    agent_label TEXT NOT NULL DEFAULT '',
    content TEXT NOT NULL DEFAULT '',
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX ux_thoughts_task_seq ON thoughts (task_id, seq);
CREATE INDEX ix_thoughts_task_id ON thoughts (task_id, seq);

-- Persistierte Ereignisse. Sie dienen der Rückblende und der Prüfung.
CREATE TABLE events (
    event_id UUID PRIMARY KEY,
    task_id UUID REFERENCES tasks (task_id) ON DELETE SET NULL,
    trace_id TEXT NOT NULL DEFAULT '',
    topic TEXT NOT NULL,
    event_type TEXT NOT NULL DEFAULT '',
    producer TEXT NOT NULL DEFAULT '',
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    meta JSONB NOT NULL DEFAULT '{}'::jsonb,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX ix_events_trace_id ON events (trace_id);
CREATE INDEX ix_events_task_id ON events (task_id);
CREATE INDEX ix_events_topic ON events (topic);
CREATE INDEX ix_events_event_type ON events (event_type);
CREATE INDEX ix_events_timestamp ON events (timestamp DESC);

-- Outbox. Ereignisse landen zuerst hier und werden danach von einem Relay
-- in Redis veröffentlicht. Damit kann eine Transaktion nicht ohne
-- ausgeliefertes Ereignis enden. Der Inhalt liegt in `events`, die Outbox
-- verweist darauf und hält nur den Zustellzustand.
CREATE TABLE outbox (
    id BIGSERIAL PRIMARY KEY,
    event_id UUID NOT NULL UNIQUE REFERENCES events (event_id) ON DELETE CASCADE,
    published BOOLEAN NOT NULL DEFAULT FALSE,
    attempts INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    published_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK ((published AND published_at IS NOT NULL) OR (NOT published))
);

CREATE INDEX ix_outbox_unpublished ON outbox (published, id);
