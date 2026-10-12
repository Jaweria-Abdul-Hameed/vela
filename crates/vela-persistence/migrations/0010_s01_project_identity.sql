-- S01: repository identity on a project, and soft removal (a removed project keeps its run history; the repository is never touched).
ALTER TABLE projects ADD COLUMN remote_url TEXT;
ALTER TABLE projects ADD COLUMN default_branch TEXT;
ALTER TABLE projects ADD COLUMN head_at_import TEXT;
ALTER TABLE projects ADD COLUMN removed_ms INTEGER;
