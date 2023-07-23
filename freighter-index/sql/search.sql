select crates.name,
       crates.description,
       array_agg(distinct cv.version) as versions,
       count(distinct concat(d.dependent, crates.id))
from crates
         join crate_versions cv on crates.id = cv.crate
         left join dependencies d on crates.id = d.dependency
where crates.registry = ''
  and position($1 in crates.name) > 0
group by crates.name, crates.description
having count(cv.version) > 0
