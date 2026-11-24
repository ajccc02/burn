begin;
create function public.prepare_feedback() returns trigger language plpgsql set search_path = public, pg_temp as $$
begin
  new.created_at := now();
  return new;
end;
$$;
create trigger feedback_prepare before insert on public.usability_feedback
  for each row execute function public.prepare_feedback();
commit;
