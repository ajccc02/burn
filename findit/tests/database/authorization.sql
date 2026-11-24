-- Technical assertions only. Every write, including Auth test identities and
-- feedback fixtures, is rolled back. Never export these as participant evidence.
begin;
insert into auth.users(id, is_anonymous) values
 ('a0110000-0000-4000-8000-000000000001',true),
 ('a0110000-0000-4000-8000-000000000002',true);
set local role authenticated;
select set_config('request.jwt.claims','{"sub":"a0110000-0000-4000-8000-000000000001","role":"authenticated","is_anonymous":true}',true);
insert into public.reports(id,user_id,report_type,item_name,category,description,location,incident_date)
values ('a0110000-0000-4000-8000-000000000003',auth.uid(),'lost','QA rollback-only calculator','Electronics','Temporary technical fixture that is rolled back.','University Library',(current_timestamp at time zone 'Asia/Manila')::date);
update public.reports set status='resolved' where id='a0110000-0000-4000-8000-000000000003';
do $$ begin
  if (select status from public.reports where id='a0110000-0000-4000-8000-000000000003') <> 'resolved' then raise exception 'Owner update failed'; end if;
  begin
    insert into public.reports(user_id,report_type,item_name,category,description,location,incident_date,is_demo)
    values(null,'lost','Unauthorized demo','Electronics','This insertion must fail.','Main Building',current_date,true);
    raise exception 'Authenticated user created a demo';
  exception when insufficient_privilege then null; end;
end $$;
select set_config('request.jwt.claims','{"sub":"a0110000-0000-4000-8000-000000000002","role":"authenticated","is_anonymous":true}',true);
do $$ declare affected integer; begin
  if not exists(select 1 from public.reports where id='a0110000-0000-4000-8000-000000000003') then raise exception 'Shared read failed'; end if;
  update public.reports set item_name='Unauthorized edit' where id='a0110000-0000-4000-8000-000000000003';
  get diagnostics affected = row_count;
  if affected <> 0 then raise exception 'Non-owner updated report'; end if;
  delete from public.reports where id='a0110000-0000-4000-8000-000000000003';
  get diagnostics affected = row_count;
  if affected <> 0 then raise exception 'Non-owner deleted report'; end if;
  update public.reports set item_name='Unauthorized demo edit' where is_demo;
  get diagnostics affected = row_count;
  if affected <> 0 then raise exception 'Demo protection failed'; end if;
  begin
    insert into storage.objects(bucket_id,name) values ('report-images','a0110000-0000-4000-8000-000000000001/a0110000-0000-4000-8000-000000000003/a0110000-0000-4000-8000-000000000004.png');
    raise exception 'Foreign storage upload accepted';
  exception when insufficient_privilege then null; end;
  insert into public.usability_feedback(id,participant_code,testing_stage,consent_given,ease_of_use,navigation,clarity,system_feedback,accessibility,task_completion,overall_satisfaction)
  values('a0110000-0000-4000-8000-000000000005','QA_ROLLBACK_ONLY','Initial',true,3,3,3,3,3,3,3);
  begin
    perform id from public.usability_feedback;
    raise exception 'Participant feedback select was permitted';
  exception when insufficient_privilege then null; end;
  begin
    insert into public.usability_feedback(participant_code,testing_stage,consent_given,ease_of_use,navigation,clarity,system_feedback,accessibility,task_completion,overall_satisfaction)
    values('QA_ROLLBACK_ONLY','Initial',true,0,3,3,3,3,3,3);
    raise exception 'Invalid rating accepted';
  exception when check_violation then null; end;
  begin
    insert into public.usability_feedback(participant_code,testing_stage,consent_given,ease_of_use,navigation,clarity,system_feedback,accessibility,task_completion,overall_satisfaction)
    values('QA_ROLLBACK_ONLY','Initial',false,3,3,3,3,3,3,3);
    raise exception 'Missing consent accepted';
  exception when check_violation or insufficient_privilege then null; end;
end $$;
set local role anon;
do $$ begin
  begin
    perform id from public.reports;
    raise exception 'Unauthenticated browse was permitted';
  exception when insufficient_privilege then null; end;
end $$;
rollback;
