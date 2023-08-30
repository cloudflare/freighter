with ins
         as (insert into crates (name, registry, created_at) values ($1, $2, current_timestamp) on conflict do nothing returning id),
     dependency_crate as
         (select id
          from ins
          union all
          select id
          from crates
          where name = $1
            and (registry = $2 or registry = '' and $2 = ''))
insert
into dependencies
(dependent, dependency, req, features, optional, default_features, target, kind, package)
VALUES ($3, (select id from dependency_crate), $4, $5, $6, $7, $8, $9, $10)
returning id;
