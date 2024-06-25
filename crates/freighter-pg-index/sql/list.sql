select c.name,
       c.description,
       c.documentation,
       c.homepage,
       c.repository,
       c.created_at,
       c.updated_at,
       array_agg(distinct cv.version)                                     as versions,
       array_agg(distinct cat.name) filter ( where cat.name is not null ) as categories,
       array_agg(distinct k.name) filter ( where k.name is not null )     as keywords
from crates c
         join crate_versions cv on c.id = cv.crate
         left join crate_categories cc on c.id = cc.crate
         left join categories cat on cat.id = cc.category
         left join crate_keywords ck on c.id = ck.crate
         left join keywords k on k.id = ck.keyword
where c.registry = ''
group by c.name, c.description, c.documentation, c.homepage, c.repository, c.created_at, c.updated_at
having count(cv.version) > 0
