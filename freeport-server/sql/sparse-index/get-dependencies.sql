select d.*, c.name, c.registry
from freeport.dependencies d
         join freeport.crates c on c.id = d.dependency
where dependent = $1