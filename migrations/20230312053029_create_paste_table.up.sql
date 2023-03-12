CREATE TABLE paste (
  id         INTEGER  PRIMARY KEY,
  key        TEXT     UNIQUE NOT NULL,
  delete_key TEXT,
  file_name  TEXT     NOT NULL,
  timestamp  DATETIME DEFAULT CURRENT_TIMESTAMP
);
