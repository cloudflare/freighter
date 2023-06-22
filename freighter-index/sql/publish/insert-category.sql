with ins as (insert into categories (name) values ($1) on conflict do nothing returning id)
select id
from ins
union all
select id
from categories
where name = $1
