select freighter_users.id, freighter_users.username
from freighter_users
         join freighter_crate_owners fco on freighter_users.id = fco.user_id
where fco.crate = $1;
