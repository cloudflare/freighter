select d.*, c.name, c.registry
from dependencies d
         join crates c on c.id = d.dependency
where dependent = $1
