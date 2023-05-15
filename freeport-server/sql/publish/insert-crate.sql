with ins as (insert into freeport.crates (name) values ($1) on conflict do nothing returning id)
select id
from ins
union all
select id
from freeport.crates
where name = $1
  and registry is null