with ins as (insert into crates (name) values ($1) on conflict do nothing returning *)
select *
from ins
union all
select *
from crates
where name = $1
