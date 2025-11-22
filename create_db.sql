CREATE TABLE actors (
  id INT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  name TEXT
);

INSERT INTO actors (name) VALUES
  ('Katie Cassidy'),
  ('Jessica Szohr');
