with ins as (insert into crates (name) values ($1) on conflict do nothing returning id)
select id
from ins
union all
select id
from crates
where name = $1
  and registry is null