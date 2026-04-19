import { useEffect, useState } from "react";
import type { HandoffTargetProfileOption } from "../chatmem-memory/types";

type HandoffComposerModalProps = {
  targetAgent: string;
  profileOptions: HandoffTargetProfileOption[];
  onClose: () => void;
  onCreate: (targetProfile: string) => void;
};

export default function HandoffComposerModal({
  targetAgent,
  profileOptions,
  onClose,
  onCreate,
}: HandoffComposerModalProps) {
  const [targetProfile, setTargetProfile] = useState(profileOptions[0]?.value ?? "");

  useEffect(() => {
    setTargetProfile(profileOptions[0]?.value ?? "");
  }, [profileOptions, targetAgent]);

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal" onClick={(event) => event.stopPropagation()}>
        <h3>Create handoff to {targetAgent}</h3>
        <div className="modal-content">
          <div className="form-group">
            <label htmlFor="handoff-target-profile">Target profile</label>
            <select
              id="handoff-target-profile"
              value={targetProfile}
              onChange={(event) => setTargetProfile(event.target.value)}
            >
              {profileOptions.map((profile) => (
                <option key={profile.value} value={profile.value}>
                  {profile.label}
                </option>
              ))}
            </select>
          </div>

          {profileOptions.length > 0 && (
            <p className="modal-helper-text">
              {
                profileOptions.find((profile) => profile.value === targetProfile)?.description
              }
            </p>
          )}
        </div>

        <div className="modal-actions">
          <button type="button" className="btn btn-secondary" onClick={onClose}>
            Cancel
          </button>
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => onCreate(targetProfile)}
            disabled={!targetProfile}
          >
            Create handoff
          </button>
        </div>
      </div>
    </div>
  );
}
