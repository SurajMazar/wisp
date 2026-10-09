-- 0008_routine_results: link a routine run to the conversation that holds its answer.

ALTER TABLE routine_runs ADD COLUMN conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL;
