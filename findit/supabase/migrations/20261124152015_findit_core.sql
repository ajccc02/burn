begin;

create table public.reports (
  id uuid primary key default gen_random_uuid(),
  user_id uuid default auth.uid() references auth.users(id) on delete cascade,
  report_type text not null check (report_type in ('lost', 'found')),
  item_name text not null check (char_length(btrim(item_name)) between 2 and 100),
  category text not null check (category in ('Electronics','School Supplies','Bags','Identification Cards','Keys','Wallets','Clothing','Accessories','Water Bottles','Others')),
  description text not null check (char_length(btrim(description)) between 10 and 2000),
  color text check (color is null or char_length(btrim(color)) between 1 and 50),
  location text not null check (char_length(btrim(location)) between 2 and 100),
  incident_date date not null check (incident_date >= date '1900-01-01' and incident_date <= (current_timestamp at time zone 'Asia/Manila')::date),
  image_path text check (image_path is null or char_length(image_path) between 1 and 300),
  additional_info text check (additional_info is null or char_length(additional_info) <= 1000),
  turned_over boolean not null default false,
  status text not null default 'active' check (status in ('active', 'resolved')),
  is_demo boolean not null default false,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  constraint reports_owner check ((is_demo and user_id is null) or (not is_demo and user_id is not null)),
  constraint lost_not_turned_over check (report_type = 'found' or not turned_over),
  constraint image_owner_path check (image_path is null or (user_id is not null and image_path ~ ('^' || user_id::text || '/' || id::text || '/[0-9a-f-]{36}\.(webp|jpg|png)$')))
);

create index reports_owner_idx on public.reports(user_id, created_at desc);
create index reports_feed_idx on public.reports(status, report_type, created_at desc, id);
create index reports_category_idx on public.reports(category, status);
create index reports_location_idx on public.reports(location);
create index reports_incident_idx on public.reports(incident_date);

create table public.usability_feedback (
  id uuid primary key default gen_random_uuid(),
  participant_code text not null check (participant_code ~ '^[A-Za-z0-9_-]{2,32}$'),
  testing_stage text not null check (testing_stage in ('Initial','Showcase')),
  consent_given boolean not null check (consent_given = true),
  ease_of_use integer not null check (ease_of_use between 1 and 5),
  navigation integer not null check (navigation between 1 and 5),
  clarity integer not null check (clarity between 1 and 5),
  system_feedback integer not null check (system_feedback between 1 and 5),
  accessibility integer not null check (accessibility between 1 and 5),
  task_completion integer not null check (task_completion between 1 and 5),
  overall_satisfaction integer not null check (overall_satisfaction between 1 and 5),
  comments text check (comments is null or char_length(comments) <= 4000),
  created_at timestamptz not null default now()
);
create index usability_feedback_stage_idx on public.usability_feedback(testing_stage, created_at);

alter table public.reports enable row level security;
alter table public.usability_feedback enable row level security;
revoke all on public.reports, public.usability_feedback from anon;
grant select, insert, update, delete on public.reports to authenticated;
revoke all on public.usability_feedback from authenticated;
grant insert on public.usability_feedback to authenticated;

create policy reports_read on public.reports for select to authenticated using (true);
create policy reports_create on public.reports for insert to authenticated
  with check (user_id = (select auth.uid()) and not is_demo);
create policy reports_edit on public.reports for update to authenticated
  using (user_id = (select auth.uid()) and not is_demo)
  with check (user_id = (select auth.uid()) and not is_demo);
create policy reports_remove on public.reports for delete to authenticated
  using (user_id = (select auth.uid()) and not is_demo);
create policy feedback_submit on public.usability_feedback for insert to authenticated
  with check (consent_given);

create function public.prepare_report() returns trigger language plpgsql set search_path = public, pg_temp as $$
begin
  if tg_op = 'UPDATE' and auth.uid() is not null and
    (new.id is distinct from old.id or new.user_id is distinct from old.user_id or
     new.is_demo is distinct from old.is_demo or new.created_at is distinct from old.created_at) then
    raise exception 'Report identity and creation fields cannot be changed' using errcode = '42501';
  end if;
  if tg_op = 'INSERT' and auth.uid() is not null then new.created_at := now(); end if;
  new.updated_at := now();
  return new;
end;
$$;
create trigger reports_prepare before insert or update on public.reports for each row execute function public.prepare_report();

insert into storage.buckets(id, name, public, file_size_limit, allowed_mime_types)
values ('report-images','report-images',false,5242880,array['image/jpeg','image/png','image/webp'])
on conflict (id) do update set public = false, file_size_limit = excluded.file_size_limit, allowed_mime_types = excluded.allowed_mime_types;

create policy report_images_read on storage.objects for select to authenticated using (
  bucket_id = 'report-images' and (
    (storage.foldername(name))[1] = (select auth.uid())::text or
    exists (select 1 from public.reports r where r.image_path = name)
  )
);
create policy report_images_upload on storage.objects for insert to authenticated with check (
  bucket_id = 'report-images' and (storage.foldername(name))[1] = (select auth.uid())::text
  and name ~ ('^' || (select auth.uid())::text || '/[0-9a-f-]{36}/[0-9a-f-]{36}\.(webp|jpg|png)$')
);
create policy report_images_delete on storage.objects for delete to authenticated using (
  bucket_id = 'report-images' and (storage.foldername(name))[1] = (select auth.uid())::text
);

create function public.check_report_image() returns trigger language plpgsql set search_path = public, storage, pg_temp as $$
begin
  if new.image_path is not null and not exists (
    select 1 from storage.objects where bucket_id = 'report-images' and name = new.image_path
  ) then
    raise exception 'The uploaded photograph could not be found. Upload it again.' using errcode = '23514';
  end if;
  return new;
end;
$$;
create trigger reports_image_check before insert or update of image_path on public.reports for each row execute function public.check_report_image();

create function public.search_reports(search_term text default '') returns setof public.reports
language sql stable security invoker set search_path = public, pg_temp as $$
  select r.* from public.reports r where char_length(search_term) <= 100 and
    (btrim(search_term) = '' or position(lower(btrim(search_term)) in lower(r.item_name || ' ' || r.description || ' ' || r.category || ' ' || r.location)) > 0);
$$;
create function public.report_counts() returns jsonb
language sql stable security invoker set search_path = public, pg_temp as $$
  select jsonb_build_object(
    'lost', count(*) filter (where report_type = 'lost' and status = 'active'),
    'found', count(*) filter (where report_type = 'found' and status = 'active'),
    'resolved', count(*) filter (where status = 'resolved'),
    'demo', count(*) filter (where is_demo)
  ) from public.reports;
$$;
create function public.report_locations() returns setof text
language sql stable security invoker set search_path = public, pg_temp as $$
  select distinct location from public.reports order by location;
$$;
create function public.match_candidates(source_id uuid) returns setof public.reports
language sql stable security invoker set search_path = public, pg_temp as $$
  select candidate.* from public.reports candidate
  join public.reports source on source.id = source_id
  where source.status = 'active' and candidate.status = 'active' and candidate.report_type <> source.report_type;
$$;
revoke all on function public.search_reports(text), public.report_counts(), public.report_locations(), public.match_candidates(uuid) from public, anon;
grant execute on function public.search_reports(text), public.report_counts(), public.report_locations(), public.match_candidates(uuid) to authenticated;

comment on table public.usability_feedback is 'Participant feedback. Export only through authorized researcher access in the Supabase dashboard. Never seed responses.';
commit;
