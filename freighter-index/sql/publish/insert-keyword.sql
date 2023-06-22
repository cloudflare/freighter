with ins as (insert into keywords (name) values ($1) on conflict do nothing returning id)
select id
from ins
union all
select id
from keywords
where name = $1
