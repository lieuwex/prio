CREATE TABLE entries (
	path TEXT NOT NULL PRIMARY KEY,
	deleted BOOLEAN NOT NULL
	--hash TEXT NOT NULL,
	--info_yaml TEXT,
	--ordering INTEGER NOT NULL DEFAULT 0,
	--checked INTEGER NOT NULL DEFAULT 0,
);

CREATE TABLE file_contents (
	path TEXT NOT NULL,
	content BLOB NOT NULL,

	at INTEGER NOT NULL,

	FOREIGN KEY (path) REFERENCES entries(path)
);
CREATE INDEX file_contents_idx ON file_contents(path);

CREATE TABLE entry_votes (
	left_path TEXT NOT NULL,
	right_path TEXT NOT NULL,
	vote INTEGER NOT NULL,

	at INTEGER NOT NULL,

	FOREIGN KEY (left_path) REFERENCES entries(path),
	FOREIGN KEY (right_path) REFERENCES entries(path)
);
CREATE INDEX entry_votes_left_path_idx ON entry_votes(left_path);
CREATE INDEX entry_votes_right_path_idx ON entry_votes(right_path);
