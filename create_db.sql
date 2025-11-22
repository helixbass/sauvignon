CREATE TABLE designers (
  id INT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  name TEXT NOT NULL
);

INSERT INTO designers (name) VALUES
  ('Proenza Schouler'),
  ('Ralph Lauren');

CREATE TABLE actors (
  id INT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  name TEXT NOT NULL,
  favorite_actor_or_designer_type TEXT NOT NULL,
  favorite_actor_or_designer_id INT NOT NULL
);

INSERT INTO actors (name, favorite_actor_or_designer_type, favorite_actor_or_designer_id) VALUES
  ('Katie Cassidy', 'designers', (SELECT id FROM designers WHERE name = 'Proenza Schouler')),
  ('Jessica Szohr', 'designers', (SELECT id FROM designers WHERE name = 'Ralph Lauren'));
