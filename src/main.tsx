import ReactDOM from "react-dom/client";
import App from "./App";
import { AppProvider } from "./context/AppContext";

import { TabsProvider } from "./context/TabsContext";
import { DialogProvider } from "./context/DialogContext";
import { KeybindingProvider } from "./context/KeybindingContext";
import { PanelProvider } from "./context/PanelContext";
import { initializeActions } from "./actions";

initializeActions();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <AppProvider>
    <TabsProvider>
      <DialogProvider>
        <KeybindingProvider>
          <PanelProvider>
            <App />
          </PanelProvider>
        </KeybindingProvider>
      </DialogProvider>
    </TabsProvider>
  </AppProvider>,
);

