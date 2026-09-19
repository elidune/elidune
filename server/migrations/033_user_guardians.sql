-- Patron-to-patron legal guardian (issue #54).
-- One guardian per minor: PRIMARY KEY on child_id makes reminder routing unambiguous.
-- Not a column on users — the child account is otherwise unchanged.

CREATE TABLE IF NOT EXISTS user_guardians (
    child_id    BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    guardian_id BIGINT NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (child_id),
    CONSTRAINT user_guardians_not_self CHECK (child_id <> guardian_id)
);

CREATE INDEX IF NOT EXISTS idx_user_guardians_guardian ON user_guardians (guardian_id);

COMMENT ON TABLE user_guardians IS
    'Legal guardian link: child_id is a minor patron (public type child or school); guardian_id is another patron.';
