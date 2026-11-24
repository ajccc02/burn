-- Optional fictional item reports. Run after the migration with researcher access.
-- Fixed UUIDs make reruns safe. This script never creates feedback responses.
insert into public.reports
  (id,user_id,report_type,item_name,category,description,color,location,incident_date,additional_info,turned_over,status,is_demo)
select id::uuid,null,report_type,item_name,category,description,color,location,
  (current_timestamp at time zone 'Asia/Manila')::date - days_ago,
  'Fictional demonstration report for the FindIt classroom prototype.',turned_over,status,true
from (values
  ('f17d0000-0000-4000-8000-000000000001','lost','Black umbrella','Others','A compact black folding umbrella with a curved handle and a small silver button.','Black','Main Building',2,false,'active'),
  ('f17d0000-0000-4000-8000-000000000002','found','Black folding umbrella','Others','A black folding umbrella was left near the Main Building entrance. It has a curved handle.','Black','Main Building',1,true,'active'),
  ('f17d0000-0000-4000-8000-000000000003','lost','Blue water bottle','Water Bottles','A blue insulated bottle with a black screw lid and a small dent near its base.','Blue','Campus Quadrangle',3,false,'active'),
  ('f17d0000-0000-4000-8000-000000000004','found','Blue insulated water bottle','Water Bottles','An insulated blue bottle with a black lid was found near the quadrangle benches.','Blue','Campus Quadrangle',2,true,'active'),
  ('f17d0000-0000-4000-8000-000000000005','lost','Black scientific calculator','Electronics','A black scientific calculator inside a sliding cover, with a faded sticker on the back.','Black','Computer Laboratory',1,false,'active'),
  ('f17d0000-0000-4000-8000-000000000006','found','Scientific calculator','Electronics','A black calculator with its sliding cover was left beside a computer. A faded sticker is on the back.','Black','Computer Laboratory',1,true,'active'),
  ('f17d0000-0000-4000-8000-000000000007','lost','Keychain with blue tag','Keys','Three silver keys on a round ring with a blank blue plastic tag.','Blue','Engineering Building',1,false,'active'),
  ('f17d0000-0000-4000-8000-000000000008','found','Keys with blue tag','Keys','A ring with three silver keys and a blank blue tag was found near a classroom.','Blue','Engineering Building',0,true,'active'),
  ('f17d0000-0000-4000-8000-000000000009','found','Gray pencil case','School Supplies','A gray zippered pencil case with two compartments and several ordinary pencils inside.','Gray','Canteen',4,true,'active'),
  ('f17d0000-0000-4000-8000-000000000010','lost','White wireless earbuds case','Electronics','A plain white oval charging case with a small scuff on the lid and no personal markings.','White','Gymnasium',2,false,'active'),
  ('f17d0000-0000-4000-8000-000000000011','found','USB flash drive','Electronics','A small silver USB flash drive with a rotating blue cover. Its contents have not been opened.','Blue','Computer Laboratory',2,true,'active'),
  ('f17d0000-0000-4000-8000-000000000012','lost','Blue notebook','School Supplies','A blue spiral notebook with a plain cover and a bent lower corner.','Blue','University Library',1,false,'active'),
  ('f17d0000-0000-4000-8000-000000000013','found','Black jacket','Clothing','A lightweight black jacket with a front zipper and two side pockets.','Black','Student Lounge',5,true,'resolved'),
  ('f17d0000-0000-4000-8000-000000000014','lost','Brown wallet','Wallets','A small brown folding wallet with a stitched edge. Personal contents are intentionally omitted.','Brown','Canteen',2,false,'active'),
  ('f17d0000-0000-4000-8000-000000000015','found','Laboratory goggles','Accessories','Clear laboratory goggles with a blue frame and an adjustable elastic strap.','Blue','Engineering Building',1,true,'active'),
  ('f17d0000-0000-4000-8000-000000000016','lost','Canvas tote bag','Bags','A cream canvas tote with a plain exterior, long handles, and a small inner pocket.','Cream','Gymnasium',6,false,'resolved')
) as demo(id,report_type,item_name,category,description,color,location,days_ago,turned_over,status)
on conflict (id) do nothing;
