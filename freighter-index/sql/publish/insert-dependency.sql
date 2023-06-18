with ins as (insert into crates_index.crates (name, registry) values ($1, $2) on conflict do nothing returning id),
     dependency_crate as
         (select id
          from ins
          union all
          select id
          from crates_index.crates
          where name = $1
            and (registry = $2 or registry is null and $2 is null))
insert
into crates_index.dependencies
(dependent, dependency, req, features, optional, default_features, target, kind, package)
VALUES ($3, (select id from dependency_crate), $4, $5, $6, $7, $8, $9, $10)
returning id;