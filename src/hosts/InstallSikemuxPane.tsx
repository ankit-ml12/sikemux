import { setUpOverSsh, sshSetUp } from "./hosts";
import { useStore } from "../state/store";
import { IconDownload, IconLogIn } from "../ui/Icons";
import { EmptyState } from "../ui/Panel";

/** What installing Sikemux on an SSH host adds, and the one action that does it (ADR 0019). */
export function InstallSikemuxPane({ alias }: { alias: string }) {
    const setUp = useStore((s) => sshSetUp(s, alias));
    const work = useStore((s) => s.sshWork[alias]);
    const signingIn = setUp === "signing in";
    const message = !setUp
        ? `${alias} runs Sikemux on another account.`
        : work
          ? `${work === "installing" ? "Installing" : "Signing in"} in the tab beside this one, where you type a password if ${alias} asks for one.`
          : signingIn
            ? `Sikemux is on ${alias} but not signed in. Signing in runs sikemux login there, in a new tab, and adds it to your account.`
            : `With Sikemux, ${alias} runs agents that keep working while you are away, your phone reaches it, and its projects, chats and files open here without SSH. The installer runs over your SSH in a new tab, where you type sudo's password if it asks.`;
    return (
        <div className="install-sikemux-pane">
            <EmptyState
                icon={signingIn ? <IconLogIn size={22} /> : <IconDownload size={22} />}
                title={signingIn ? `Sign in ${alias}` : `Install Sikemux on ${alias}`}
                message={message}
                action={setUp && !work ? { label: signingIn ? "Sign in" : "Install Sikemux", onClick: () => setUpOverSsh(alias, setUp) } : undefined}
            />
        </div>
    );
}
