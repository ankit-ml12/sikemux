import { lazy } from "react";
import { registerFrontendPlugin, signInOf } from "../../plugin-api";
import { useResource } from "../../plugin-api/resources";
import { AzureMark } from "./components/AzureMark";
import { AZURE_PLUGIN_ID, AZURE_PORTAL } from "./kinds";
import { azureStatusR } from "./resources";
import { openAzure } from "./state";

const AzurePane = lazy(() => import("./components/AzurePane").then((module) => ({ default: module.AzurePane })));

registerFrontendPlugin({
    id: AZURE_PLUGIN_ID,
    surfaces: [
        {
            kind: AZURE_PORTAL,
            title: "Azure",
            icon: (size) => <AzureMark size={size} className="icon-azure" />,
            render: ({ paneId, visible }) => <AzurePane paneId={paneId} active={visible} />,
        },
    ],
    open: openAzure,
    openTitle: "Open Azure",
    mark: (size) => <AzureMark size={size} className="icon-azure" />,
    linkHosts: ["portal.azure.com"],
    useSignIn: function useServiceSignIn() {
        return signInOf(useResource(azureStatusR));
    },
});
