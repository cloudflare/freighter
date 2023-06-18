select d.*, c.name, c.registry
from crates_index.dependencies d
         join crates_index.crates c on c.id = d.dependency
where dependent = $1