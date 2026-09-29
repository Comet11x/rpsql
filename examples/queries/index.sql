-- The entry point of the queries directory. A directory is read as the
-- index.sql it holds, so this file is built by
--
--     rpsql -i queries -o out.sql
--
-- It sets everything it needs, so it can also be built on its own.

@set table = orders
@set columns = id, user_id, total, created_at
@set order_by = created_at desc

-- An include is resolved against the directory of the file that includes it,
-- and a directory is read as its index.sql as well.
@include rows.sql

-- A query against another table. The variables of the file above stay in that
-- file, so this one uses the ones set at the top.
@set table = users
select id, email from ${table} where created_at > now() - interval '7 days';
