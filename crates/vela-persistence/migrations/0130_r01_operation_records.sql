-- R01: every external operation records its intent before and its observed result after, and names the recipe that reconciles it.
ALTER TABLE operations ADD COLUMN intent TEXT NOT NULL DEFAULT '{}';
ALTER TABLE operations ADD COLUMN result TEXT;
ALTER TABLE operations ADD COLUMN recipe TEXT;
CREATE INDEX operations_state ON operations(state, kind);
