import { lazy } from "react";
import { registerFrontendPlugin, signInOf } from "../../plugin-api";
import { useResource } from "../../plugin-api/resources";
import { SentryMark } from "./components/SentryMark";
import { SENTRY_ISSUES, SENTRY_PLUGIN_ID } from "./kinds";
import { sentryStatusR } from "./resources";
import { openSentry } from "./state";

const SentryPane = lazy(() => import("./components/SentryPane").then((module) => ({ default: module.SentryPane })));

registerFrontendPlugin({
    id: SENTRY_PLUGIN_ID,
    surfaces: [
        {
            kind: SENTRY_ISSUES,
            title: "Sentry",
            icon: (size) => <SentryMark size={size} className="icon-sentry" />,
            render: ({ paneId, visible }) => <SentryPane paneId={paneId} active={visible} />,
        },
    ],
    open: openSentry,
    openTitle: "Open Sentry",
    mark: (size) => <SentryMark size={size} className="icon-sentry" />,
    linkHosts: ["sentry.io"],
    useSignIn: function useServiceSignIn() {
        return signInOf(useResource(sentryStatusR));
    },
});
