CREATE TABLE entries (
	path TEXT NOT NULL PRIMARY KEY,
	hash TEXT NOT NULL,
	info_yaml TEXT,
	ordering INTEGER NOT NULL DEFAULT 0,
	checked INTEGER NOT NULL DEFAULT 0,

	added_at INTEGER NOT NULL,
	updated_at INTEGER NOT NULL
);
CREATE INDEX entries_ordering_idx ON entries(ordering);
