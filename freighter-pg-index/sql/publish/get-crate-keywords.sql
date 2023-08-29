select keywords.name
from crate_keywords
         join keywords
              on keywords.id = crate_keywords.keyword
where crate_keywords.crate = $1
