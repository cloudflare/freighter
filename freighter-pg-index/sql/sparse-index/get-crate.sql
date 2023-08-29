select id
from crates
where name = $1
  and registry = ''
