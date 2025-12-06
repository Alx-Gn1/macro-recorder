import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import { formatFileList } from "./utils/utils";
import { FileList, FormattedFileList } from "./utils/types";
import MacroViewer from "./Component/MacroViewer";
import { listen } from "@tauri-apps/api/event";

function App() {
  const [, setAppConfigPath] = useState("");
  const [macroFilesList, setMacroFilesList] = useState<FormattedFileList>([]);
  const [selectedMacro, setSelectedMacro] = useState<string | null>(null);
  const [refreshApp, setRefreshApp] = useState<boolean>(false);

  // utils function
  async function selectMacro(fileName: string) {
    // select macro in backend
    const selectedMacroName: string = await invoke("select_macro", { name: fileName });
    setSelectedMacro(selectedMacroName);
  }
  const fetchAppConfig = async () => {
    const configPath: string = await invoke("get_app_config_path");
    const files_in_folder: FileList = await invoke("list_files_from_config", { path: configPath });
    const formattedFileList = formatFileList(files_in_folder);
    setMacroFilesList(formattedFileList);
    setAppConfigPath(configPath);
    if (formattedFileList.length > 0) {
      selectMacro(formattedFileList[0].fileName);
    }
  };

  // Get config folder path on app launch
  useEffect(() => {
    fetchAppConfig();
  }, [refreshApp]);

  // function to fetch file list whenever a new macro is recorded
  useEffect(() => {
    const unlisten = listen("refresh-app", () => {
      fetchAppConfig();
      setRefreshApp(!refreshApp);
    });

    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  return (
    <main className="container">
      {/* <p>Config path : {appConfigPath}</p> */}
      <h1>Rust Macro Recorder</h1>
      {macroFilesList.length > 0 ? (
        <>
          <div id="table-container">
            <table>
              <thead>
                <tr>
                  <th>Name</th>
                  <th>Duration</th>
                  <th>Date</th>
                </tr>
              </thead>
              <tbody>
                {macroFilesList.map((file) => (
                  <tr
                    key={file.fileName}
                    onClick={() => {
                      selectMacro(file.fileName);
                    }}
                  >
                    <td>{file.displayName}</td>
                    <td>{file.durationInSec + " s"}</td>
                    <td>{file.date.toLocaleString("fr-FR")}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <MacroViewer
            fileList={macroFilesList}
            currentSelection={selectedMacro}
            setRefreshApp={setRefreshApp}
            refreshApp={refreshApp}
          />
        </>
      ) : (
        <div>
          <p>You don't have recorded any macro yet</p>
          <p>press Ctrl + Shift + R to start recording</p>
          <p>press Ctrl + Shift + R again to stop the recording</p>
        </div>
      )}
    </main>
  );
}

export default App;
