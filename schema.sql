CREATE TABLE entries (
	path TEXT NOT NULL PRIMARY KEY,
	deleted BOOLEAN NOT NULL
	--hash TEXT NOT NULL,
	--info_yaml TEXT,
	--ordering INTEGER NOT NULL DEFAULT 0,
	--checked INTEGER NOT NULL DEFAULT 0,
);

CREATE TABLE hash_changes (
	path TEXT NOT NULL,
	hash INTEGER NOT NULL,

	at INTEGER NOT NULL,

	FOREIGN KEY (path) REFERENCES entries(path)
);
CREATE INDEX hash_changes_path_idx ON hash_changes(path);

CREATE TABLE entry_ordering (
	left_path TEXT NOT NULL,
	right_path TEXT NOT NULL,
	vote INTEGER NOT NULL,

	at INTEGER NOT NULL,

	FOREIGN KEY (left_path) REFERENCES entries(path),
	FOREIGN KEY (right_path) REFERENCES entries(path)
);
CREATE INDEX entry_ordering_left_path_idx ON entry_ordering(left_path);
CREATE INDEX entry_ordering_right_path_idx ON entry_ordering(right_path);
