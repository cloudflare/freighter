delete
from crate_categories
    using categories
where crate = $1
  and category = categories.id
  and categories.name = $2
