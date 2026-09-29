-- The entry point of the example, build it with:
--
--     rpsql -i main.sql -o out.sql
--
-- Every directive is removed from the result and every `--` comment is
-- dropped, so out.sql is plain SQL that a database client can run.

@set table = users
@set columns = id, name, email
@set order_by = created_at desc

-- An include inlines a file. It is resolved against the directory of the file
-- that includes it, and the variables of this file are visible in the file it
-- includes, so the query below is built from ${table}, ${columns} and
-- ${order_by}.
@include queries/rows.sql

-- The scope of a file ends with the file itself, so the variables of
-- queries/rows.sql are not visible here and this query uses the ones set
-- above.
select count(*) as total from ${table};

-- @set global promotes a variable to the global scope, where every file can
-- reach it. Its value is expanded before it is stored, so a variable can be
-- built out of other ones.
@set global schema = public
select ${columns} from ${schema}.${table} order by ${order_by};

-- @unset drops a name from the file. Every directive of a file is applied
-- before any of its variables is replaced, so an unset name is gone from the
-- whole file and not only from the lines that follow it. An unset name is
-- unknown even when the global scope still knows it.
@set limit_clause = limit 20
@unset limit_clause
select ${columns} from ${table} where email is not null;
