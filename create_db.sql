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
  expression TEXT NOT NULL,
  favorite_actor_or_designer_type TEXT NOT NULL,
  favorite_actor_or_designer_id INT NOT NULL,
  favorite_designer_id INT NOT NULL
);

INSERT INTO actors (name, expression, favorite_actor_or_designer_type, favorite_actor_or_designer_id, favorite_designer_id) VALUES
  ('Katie Cassidy', 'no Serena you can''t have the key', 'designers', (SELECT id FROM designers WHERE name = 'Proenza Schouler'), (SELECT id FROM designers WHERE name = 'Proenza Schouler'));
INSERT INTO actors (name, expression, favorite_actor_or_designer_type, favorite_actor_or_designer_id, favorite_designer_id) VALUES
  ('Jessica Szohr', 'Dan where did you go I don''t like you', 'actors', (SELECT id FROM actors WHERE name = 'Katie Cassidy'), (SELECT id FROM designers WHERE name = 'Ralph Lauren'));
