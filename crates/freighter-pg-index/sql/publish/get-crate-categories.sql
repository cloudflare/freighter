select categories.name
from crate_categories
         join categories
              on categories.id = crate_categories.category
where crate_categories.crate = $1
