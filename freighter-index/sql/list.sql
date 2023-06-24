select crates.name,
       crates.description,
       crates.documentation,
       crates.homepage,
       crates.repository,
       array_agg(cv.version)      as versions,
       count(dependency),
       array_agg(distinct c.name) as categories,
       array_agg(distinct k.name) as keywords
from crates
         join crate_versions cv on crates.id = cv.crate
         left join dependencies d on crates.id = d.dependency
         left join crate_categories cc on crates.id = cc.crate
         join categories c on c.id = cc.category
         left join crate_keywords ck on crates.id = ck.crate
         join keywords k on k.id = ck.keyword
where crates.registry is null
group by crates.name, crates.description, crates.documentation, crates.homepage, crates.repository
having count(cv.version) > 0
