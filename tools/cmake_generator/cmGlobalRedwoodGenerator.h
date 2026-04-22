/* Distributed under the OSI-approved BSD 3-Clause License.  */
#pragma once

#include "cmGlobalCommonGenerator.h"
#include <iosfwd>
#include <memory>
#include <string>
#include <vector>

class cmGeneratorTarget;
class cmLocalGenerator;

/**
 * CMake generator for Redwood Build System
 * Generates BUILD.datalog files compatible with Redwood
 */
class cmGlobalRedwoodGenerator : public cmGlobalCommonGenerator
{
public:
  cmGlobalRedwoodGenerator(cmake* cm);
  ~cmGlobalRedwoodGenerator() override = default;

  static std::unique_ptr<cmGlobalGeneratorFactory> NewFactory();

  std::string GetName() const override { return "Redwood"; }

  bool SupportsToolset() const override { return false; }
  bool SupportsPlatform() const override { return false; }

  void Generate() override;

  void EnableLanguage(std::vector<std::string> const& languages,
                      cmMakefile* mf, bool optional) override;

protected:
  void GenerateBuildCommand(GeneratedMakeCommand& makeCommand,
                            const std::string& makeProgram,
                            const std::string& projectName,
                            const std::string& projectDir,
                            std::vector<std::string> const& targetNames,
                            const std::string& config, bool fast, int jobs,
                            bool verbose,
                            std::vector<std::string> const& makeOptions =
                              std::vector<std::string>()) override;

  std::unique_ptr<cmLocalGenerator> CreateLocalGenerator(
    cmMakefile* mf) override;

private:
  void WriteDatalogFile(std::ostream& os);
  void WriteTarget(std::ostream& os, cmGeneratorTarget* gt);
  void WriteTargetFacts(std::ostream& os, const std::string& targetLabel,
                        cmGeneratorTarget* gt);
  void WriteSourceFacts(std::ostream& os, const std::string& targetLabel,
                        cmGeneratorTarget* gt);
  void WriteCompileFlags(std::ostream& os, const std::string& targetLabel,
                         cmGeneratorTarget* gt);
  void WriteDependencies(std::ostream& os, const std::string& targetLabel,
                         cmGeneratorTarget* gt);

  std::string EscapeDatalogString(const std::string& str) const;
  std::string GenerateTargetLabel(cmGeneratorTarget* gt) const;
};
