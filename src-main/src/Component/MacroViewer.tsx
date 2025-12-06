import React, { useState } from "react";
import { FormattedFileList } from "../utils/types";
import Popup from "reactjs-popup";
import "reactjs-popup/dist/index.css";
import { deleteFileFromName } from "../utils/utils";
import { invoke } from "@tauri-apps/api/core";

type Props = {
  fileList: FormattedFileList;
  currentSelection: string | null;
  setRefreshApp: React.Dispatch<React.SetStateAction<boolean>>;
  refreshApp: boolean;
};

const MacroViewer = ({ fileList, currentSelection, setRefreshApp, refreshApp }: Props) => {
  const [open, setOpen] = useState<boolean>(false);

  const selectedMacro = fileList.find((file) => file.fileName === currentSelection);

  const contentAsObj: Array<any> = selectedMacro?.content ? JSON.parse(selectedMacro?.content) : JSON.parse("[]");

  return (
    <div id="macro-viewer-container">
      {selectedMacro ? (
        <>
          <div id="viewer-main">
            <div id="viewer-left-side">
              <table>
                <tbody>
                  <tr>
                    <td>Selected Macro</td>
                    <td>{selectedMacro.displayName}</td>
                  </tr>
                  <tr>
                    <td>Creation Date</td>
                    <td>{selectedMacro.date.toLocaleString("fr-FR")}</td>
                  </tr>
                  <tr>
                    <td>Duration</td>
                    <td>{selectedMacro.durationInSec + " s"}</td>
                  </tr>
                </tbody>
              </table>
              <button onClick={() => setOpen(true)}>Delete Macro</button>
              <Popup modal open={open} onClose={() => setOpen(false)}>
                <div className="delete-modal">
                  <p>Warning : Deleting a macro is irreversible</p>
                  <div>
                    <button onClick={() => setOpen(false)}>Cancel</button>
                    <button
                      onClick={() => {
                        deleteFileFromName(selectedMacro.fileName).then(() => {
                          setRefreshApp(!refreshApp);
                          setOpen(false);
                        });
                      }}
                    >
                      Confirm Deletion
                    </button>
                  </div>
                </div>
              </Popup>

              <button
                onClick={async () => {
                  const configPath: string = await invoke("get_app_config_path");
                  await invoke("open_file_browser", { path: configPath });
                }}
              >
                Open file location
              </button>
            </div>
            <div id="viewer-right-side">
              <p>File content :</p>
              <textarea className="file-content" value={JSON.stringify(contentAsObj, null, 4)} readOnly={true}></textarea>
            </div>
          </div>
          <footer>
            <p>
              <strong>Ctrl + Shift + X</strong> <br />
              start executing the macro
            </p>
            <p>
              <strong>Ctrl + Shift + C</strong> <br />
              stop the macro during execution
            </p>
          </footer>
        </>
      ) : (
        <p>Select a macro</p>
      )}
    </div>
  );
};

export default MacroViewer;
