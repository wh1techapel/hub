-- Studio revision R2 (docs/studio.md, "Revision: three stages"): each ledger item says which part
-- of the idea it settles, so Ringkasan can show "Yang sudah jelas" per part and group by it.
--   problem      Masalah
--   users        Pengguna
--   features     Fitur inti
--   constraints  Batasan
--   success      Ukuran berhasil
-- NULL is an item from before this migration, or one that fits none of them. Adding a column keeps
-- every row as it is: nothing is rebuilt, nothing is moved.
ALTER TABLE studio_ledger ADD COLUMN area TEXT
  CHECK (area IS NULL OR area IN ('problem', 'users', 'features', 'constraints', 'success'));
