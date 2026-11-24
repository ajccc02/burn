"use client";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { Turnstile, type TurnstileInstance } from "@marsidev/react-turnstile";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { ShieldCheck, Fingerprint } from "lucide-react";
import { browserSupabase } from "@/lib/supabase/client";
import { hasSupabaseConfig } from "@/lib/supabase/config";
import { sessionIdentity } from "@/app/actions";
import { Alert, Button, Loading } from "./ui";
type GuestState =
  "checking" | "captcha" | "signing" | "ready" | "error" | "setup";
interface Guest {
  state: GuestState;
  userId: string | null;
  error: string | null;
  retry: () => void;
  signIn: (token: string) => Promise<void>;
}
const Context = createContext<Guest | null>(null);
export function useGuest() {
  const guest = useContext(Context);
  if (!guest) throw new Error("Guest provider is missing");
  return guest;
}
export function GuestProvider({ children }: { children: ReactNode }) {
  const pathname = usePathname();
  const needsGuest =
    pathname === "/" ||
    pathname === "/browse" ||
    pathname === "/feedback" ||
    pathname.startsWith("/items/") ||
    pathname.startsWith("/my-reports") ||
    pathname === "/report/lost" ||
    pathname === "/report/found";
  const [state, setState] = useState<GuestState>("checking");
  const [userId, setUserId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const busy = useRef(false);
  const signIn = useCallback(async (token: string) => {
    if (busy.current) return;
    busy.current = true;
    setState("signing");
    setError(null);
    try {
      const client = browserSupabase();
      const result = await client.auth.signInAnonymously(
        token ? { options: { captchaToken: token } } : undefined,
      );
      if (result.error)
        throw new Error(
          result.error.status === 429
            ? "Guest access is temporarily rate-limited. Wait a few minutes and retry."
            : "Guest access could not be started. Check your connection, CAPTCHA, and Supabase anonymous sign-in settings.",
        );
      const verified = await sessionIdentity();
      if (!verified.ok) throw new Error(verified.error);
      setUserId(verified.data);
      setState("ready");
    } catch (issue) {
      setError(
        issue instanceof Error
          ? issue.message
          : "Could not establish guest access.",
      );
      setState("error");
    } finally {
      busy.current = false;
    }
  }, []);
  const restore = useCallback(async () => {
    if (busy.current) return;
    setError(null);
    setState("checking");
    if (!hasSupabaseConfig()) {
      setState("setup");
      return;
    }
    try {
      const { data, error } = await browserSupabase().auth.getSession();
      if (error) throw error;
      if (data.session) {
        const verified = await sessionIdentity();
        if (!verified.ok) throw new Error(verified.error);
        setUserId(verified.data);
        setState("ready");
        return;
      }
      if (process.env.NEXT_PUBLIC_TURNSTILE_SITE_KEY) {
        setState("captcha");
        return;
      }
      if (process.env.NODE_ENV === "production") {
        setError(
          "Guest access needs CAPTCHA configuration. Add the Turnstile site key and enable CAPTCHA in Supabase Auth.",
        );
        setState("setup");
        return;
      }
      await signIn("");
    } catch (issue) {
      setError(
        issue instanceof Error
          ? issue.message
          : "Your guest session could not be restored.",
      );
      setState("error");
    }
  }, [signIn]);
  useEffect(() => {
    let active = true;
    queueMicrotask(() => {
      if (active && needsGuest) void restore();
    });
    if (!hasSupabaseConfig())
      return () => {
        active = false;
      };
    const { data } = browserSupabase().auth.onAuthStateChange(
      (event, session) => {
        if (event === "SIGNED_OUT") {
          setUserId(null);
          setState("error");
          setError(
            "Your guest session ended. Start guest access again to continue.",
          );
        }
        if (event === "TOKEN_REFRESHED" && session) setUserId(session.user.id);
      },
    );
    return () => {
      active = false;
      data.subscription.unsubscribe();
    };
  }, [restore, needsGuest]);
  return (
    <Context.Provider
      value={{ state, userId, error, retry: () => void restore(), signIn }}
    >
      {children}
    </Context.Provider>
  );
}
export function GuestGate({ children }: { children: ReactNode }) {
  const guest = useGuest();
  const captcha = useRef<TurnstileInstance | null>(null);
  const [challengeError, setChallengeError] = useState<string | null>(null);
  if (guest.state === "ready") return children;
  if (guest.state === "checking" || guest.state === "signing")
    return (
      <Loading
        label={
          guest.state === "signing"
            ? "Starting secure guest access…"
            : "Restoring your guest session…"
        }
      />
    );
  return (
    <section className="guest-panel card">
      <div className="feature-icon">
        <Fingerprint size={28} aria-hidden />
      </div>
      <h1>
        {guest.state === "setup"
          ? "FindIt setup is needed"
          : "Welcome to FindIt"}
      </h1>
      <p className="text-muted">
        Browse and report campus belongings with a guest session. You can start
        without a name, email, or password.
      </p>
      {guest.error && <Alert>{guest.error}</Alert>}
      {guest.state === "setup" && (
        <Alert kind="info">
          The application needs its Supabase connection
          {!hasSupabaseConfig() ? " and database migration" : ""}. Project setup
          instructions are in the FindIt README.
        </Alert>
      )}
      {guest.state === "captcha" && (
        <>
          <p>Complete this quick verification to continue.</p>
          <div className="captcha-box">
            <Turnstile
              ref={captcha}
              siteKey={process.env.NEXT_PUBLIC_TURNSTILE_SITE_KEY!}
              options={{ theme: "light", size: "flexible" }}
              onSuccess={(token) => {
                setChallengeError(null);
                void guest.signIn(token);
              }}
              onExpire={() => {
                setChallengeError(
                  "The verification expired. Please try it again.",
                );
              }}
              onError={() => {
                setChallengeError(
                  "The verification could not load. Check your connection and try again.",
                );
              }}
            />
          </div>
          {challengeError && (
            <Alert>
              {challengeError}
              <Button
                variant="ghost"
                onClick={() => {
                  setChallengeError(null);
                  captcha.current?.reset();
                }}
              >
                Retry verification
              </Button>
            </Alert>
          )}
        </>
      )}
      {guest.state === "error" && (
        <Button onClick={guest.retry}>Try guest access again</Button>
      )}
      <p className="privacy-note">
        <ShieldCheck size={18} aria-hidden />
        Your reports belong to this browser&apos;s guest session. Clearing
        browser data or switching devices can remove your access to them.
      </p>
      <Link href="/about" className="text-link">
        How FindIt works
      </Link>
    </section>
  );
}
