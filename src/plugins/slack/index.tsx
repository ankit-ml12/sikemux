import { lazy } from "react";
import { registerFrontendPlugin, signInOf } from "../../plugin-api";
import { useResource } from "../../plugin-api/resources";
import { slackStatusR } from "./resources";
import { SlackMark } from "./components/SlackMark";
import { SLACK_MESSAGES, SLACK_PLUGIN } from "./kinds";
import { openSlack } from "./state";

const SlackPane = lazy(() => import("./components/SlackPane").then((module) => ({ default: module.SlackPane })));

registerFrontendPlugin({
    id: SLACK_PLUGIN.id,
    surfaces: [
        {
            kind: SLACK_MESSAGES,
            title: SLACK_PLUGIN.name,
            icon: (size) => <SlackMark size={size} />,
            render: ({ paneId, visible }) => <SlackPane paneId={paneId} active={visible} />,
        },
    ],
    open: openSlack,
    openTitle: `Open ${SLACK_PLUGIN.name}`,
    mark: (size) => <SlackMark size={size} />,
    linkHosts: [".slack.com"],
    useSignIn: function useServiceSignIn() {
        return signInOf(useResource(slackStatusR));
    },
});
